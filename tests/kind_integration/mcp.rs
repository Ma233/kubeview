use std::net::TcpListener;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use serde_json::Value;
use serde_json::json;

pub(crate) struct TestServer {
    child: Child,
    url: String,
}

impl TestServer {
    pub(crate) fn start(extra_args: &[&str]) -> anyhow::Result<Self> {
        let port = free_port()?.to_string();
        let url = format!("http://127.0.0.1:{port}/mcp");
        let mut args = vec![
            "serve",
            "--host",
            "127.0.0.1",
            "--port",
            &port,
            "--path",
            "/mcp",
        ];
        args.extend_from_slice(extra_args);

        let child = Command::new(env!("CARGO_BIN_EXE_kubeview"))
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let server = Self { child, url };
        wait_for_mcp_initialize(server.url())?;
        Ok(server)
    }

    pub(crate) fn url(&self) -> &str {
        &self.url
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub(crate) struct McpClient {
    url: String,
    protocol_version: String,
    session_id: String,
}

impl McpClient {
    pub(crate) fn connect(url: &str) -> anyhow::Result<Self> {
        let response = curl_json(url, &[], &initialize_request(1))?;
        if response.status != 200 {
            anyhow::bail!(
                "initialize failed with {}: {}",
                response.status,
                response.body
            );
        }
        let session_id = header_value(&response.headers, "mcp-session-id");
        let Some(session_id) = session_id else {
            anyhow::bail!(
                "initialize response did not include mcp-session-id: {}",
                response.body
            );
        };
        let initialize = first_json_rpc_message(&response.body)?;
        let protocol_version = initialize["result"]["protocolVersion"]
            .as_str()
            .unwrap_or("2025-03-26")
            .to_string();

        let client = Self {
            url: url.to_string(),
            protocol_version,
            session_id,
        };
        client.send_initialized()?;

        Ok(client)
    }

    pub(crate) fn call_tool(&self, name: &str, arguments: Value) -> anyhow::Result<ToolResult> {
        let response = curl_json(
            &self.url,
            &[
                ("Mcp-Protocol-Version", &self.protocol_version),
                ("Mcp-Session-Id", &self.session_id),
            ],
            &json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": name,
                    "arguments": arguments,
                },
            }),
        )?;
        if response.status != 200 {
            anyhow::bail!(
                "tool {name} failed with HTTP {}: {}",
                response.status,
                response.body
            );
        }

        let value = first_json_rpc_message(&response.body)?;
        Ok(ToolResult(json_rpc_result(&value)?.clone()))
    }

    fn send_initialized(&self) -> anyhow::Result<()> {
        let response = curl_json(
            &self.url,
            &[
                ("Mcp-Protocol-Version", &self.protocol_version),
                ("Mcp-Session-Id", &self.session_id),
            ],
            &json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }),
        )?;
        if response.status != 202 {
            anyhow::bail!(
                "initialized notification failed with {}: {}",
                response.status,
                response.body
            );
        }
        Ok(())
    }
}

fn json_rpc_result(value: &Value) -> anyhow::Result<&Value> {
    let response = value
        .as_array()
        .and_then(|items| items.first())
        .unwrap_or(value);
    response
        .get("result")
        .ok_or_else(|| anyhow::anyhow!("JSON-RPC response missing result: {value}"))
}

#[derive(Debug)]
pub(crate) struct ToolResult(Value);

impl ToolResult {
    pub(crate) fn is_error(&self) -> bool {
        self.0["isError"].as_bool().unwrap_or(false)
    }

    pub(crate) fn structured(&self) -> &Value {
        &self.0["structuredContent"]
    }

    pub(crate) fn content_text(&self) -> String {
        self.0["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| item["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

struct HttpResponse {
    status: u16,
    headers: String,
    body: String,
}

fn wait_for_mcp_initialize(url: &str) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        let response = curl_json(url, &[], &initialize_request(0));
        if let Ok(response) = response
            && response.status == 200
        {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(250));
    }
    anyhow::bail!("server at {url} did not accept MCP initialize")
}

fn curl_json(url: &str, headers: &[(&str, &str)], body: &Value) -> anyhow::Result<HttpResponse> {
    let mut command = Command::new("curl");
    command.args([
        "-sS",
        "-i",
        "-w",
        "\n__KUBEVIEW_HTTP_STATUS__:%{http_code}",
        "-H",
        "Content-Type: application/json",
        "-H",
        "Accept: application/json, text/event-stream",
    ]);
    for (name, value) in headers {
        command.args(["-H", &format!("{name}: {value}")]);
    }
    command.args(["--data", &body.to_string(), url]);

    let output = command.output()?;
    if !output.status.success() {
        anyhow::bail!(
            "curl failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let output = String::from_utf8(output.stdout)?;
    let Some((raw_response, status)) = output.rsplit_once("\n__KUBEVIEW_HTTP_STATUS__:") else {
        anyhow::bail!("curl response missing status marker: {output}");
    };
    let status = status.trim().parse::<u16>()?;
    let Some((headers, body)) = raw_response
        .split_once("\r\n\r\n")
        .or_else(|| raw_response.split_once("\n\n"))
    else {
        anyhow::bail!("curl response missing header/body separator: {raw_response}");
    };

    Ok(HttpResponse {
        status,
        headers: headers.to_string(),
        body: body.to_string(),
    })
}

fn first_json_rpc_message(body: &str) -> anyhow::Result<Value> {
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        return Ok(value);
    }

    let normalized = body.replace("\r\n", "\n");
    let mut messages = Vec::new();
    for event in normalized.split("\n\n") {
        let mut data_lines = Vec::new();
        for line in event.lines() {
            if let Some(data) = line.strip_prefix("data:") {
                data_lines.push(data.trim_start());
            }
        }
        if data_lines.is_empty() {
            continue;
        }

        let payload = data_lines.join("\n");
        if payload.trim().is_empty() {
            continue;
        }

        messages.push(serde_json::from_str::<Value>(&payload)?);
    }

    messages
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("response body did not contain a JSON-RPC message: {body}"))
}

fn header_value(headers: &str, name: &str) -> Option<String> {
    headers.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name)
            .then(|| value.trim().to_string())
    })
}

fn initialize_request(id: i64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {
                "name": "kubeview-kind-integration",
                "version": "1.0",
            },
        },
    })
}

fn free_port() -> anyhow::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}
