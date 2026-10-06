# HTTP/1.1 client for http and https, with DNS and TLS.
# ponytail: the hub takes the first comment in path order, and dns/ sorts before http.bend.
# bise patches (keep them on a vendor refresh): Wire.send.file / Wire.tls.send.file and
# Http.file_part (77615a8a: images sent from their files), Http.stream.poll (issue 13: a
# read with its own short wait that keeps a silent stream open, so a cancel is seen).
