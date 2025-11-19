# risc-v-sim-web

Web version of [risc-v-sim](https://github.com/nup-csai/risc-v-sim)

## How to run
```bash
git clone --recursive https://github.com/robocy-lab/risc-v-sim-web
cd risc-v-sim-web
docker build -t meow .
docker run -d --rm -p 3000:3000 -t meow
```

## How to use
http://localhost:3000/api/health should return `Ok`

http://localhost:3000/api/submit with POST request and `ticks=<ticks>` (text/plain) and `file=<program.s>` (application/octet-stream) should return json if all is ok
