# Crawl

A simple cli webcrawler.

## Usage

from `crawl --help`:

```text
crawl 1.0
Nathan Kolpa <nathan@kolpa.me>
A simple cli webcrawler.

USAGE:
    crawl.exe [OPTIONS] <URL>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -j, --jobs <jobs>                            The maximum amount of parallel jobs. [default: 8]
    -d, --max-depth <max-depth>
    -o, --max-origin-depth <max-origin-depth>     [default: 1]
    -u, --user-agent <user-agent>
            The user agent that will be sent along with every request [default: Crawl]


ARGS:
    <URL>    The start url
```

