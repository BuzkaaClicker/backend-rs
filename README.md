Deploy:
```bash
$ docker build --tag bclicker-server .
$ docker save bclicker-server | bzip2 | ssh -C makincc docker load
```
