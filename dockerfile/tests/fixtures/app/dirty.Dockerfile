FROM busybox:1.36
RUN echo $HOME | wc -c
