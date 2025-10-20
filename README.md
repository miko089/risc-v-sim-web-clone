# risc-v-sim-web

Web version of [risc-v-sim](https://github.com/nup-csai/risc-v-sim)

## How to run
```bash
docker build -t meow .
docker run -d --rm -p 3000:300 -t meow
```

## How to use
http://localhost:3000/health should return `Ok`

http://localhost:3000/submit with POST request and `ticks=<ticks>` (text/plain) and `file=<program.s>` (application/octet-stream) should return json if all is ok
