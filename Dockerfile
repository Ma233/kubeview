FROM alpine:3.21

ARG TARGETPLATFORM

ENV RUST_LOG=info
ENV KUBECONFIG=/home/kubeview/.kube/config

WORKDIR /opt/kubeview
RUN adduser -D kubeview --uid 1573 \
    && mkdir -p /home/kubeview/.kube \
    && chown -R kubeview:kubeview /opt/kubeview /home/kubeview

COPY ./${TARGETPLATFORM}/kubeview /usr/bin/kubeview

USER kubeview
EXPOSE 3000
ENTRYPOINT ["/usr/bin/kubeview"]
