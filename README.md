# Crawl

A simple cli webcrawler.

## Usage

from `crawl --help`:

```text
crawl 1.0.0
Nathan Kolpa <nathan@kolpa.me>
A simple cli webcrawler.

USAGE:
    crawl [OPTIONS] <URL>

FLAGS:
        --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -j, --jobs <number>                The maximum amount of parallel jobs. [default: 8]
    -d, --max-depth <number>           
    -o, --max-origin-depth <number>     [default: 1]
    -s, --only-subdirs <boolean>       Crawl only sub directories from the URL [default: false]  [possible values: true,
                                       false]
    -r, --respect-robots <boolean>     Respect robots.txt [default: true]  [possible values: true, false]
    -h, --head <boolean>               Send a HEAD request first before sending a GET request [default: true]  [possible
                                       values: true, false]
    -u, --user-agent <text>            The User-Agent header that will be sent along with every request [default: Crawl]

ARGS:
    <URL>    The start url

```

