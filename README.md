# iwicrawl - an indexer for open directories
Inspired by [a different project](https://github.com/KoalaBear84/OpenDirectoryDownloader), I set out to write a clone with less features, less polish, and less sophisticated parsing.
The result is a command line tool that you feed a url, and it barfs out all subdirectories and files listed, together with their corresponding sizes in bytes.
A bit like the unix tool `du`, but for open directories.

# Example
```
$ iwicrawl --help
iwicrawl 0.1.0
iwikal <e.joel.nordstrom@gmail.com>

USAGE:
    iwicrawl [FLAGS] <URL>

FLAGS:
    -h, --help       Prints help information
    -q, --quiet      Don't print anything except standard output
    -V, --version    Prints version information
    -v, --verbose    Increase message verbosity for each occurrance of the flag

ARGS:
    <URL>    The url of the directory to crawl
```

```
$ iwicrawl localhost
6                    http://localhost/file2
327                  http://localhost/file1
15                   http://localhost/subdir/file3
878841856            http://localhost/subdir/file4
878841871            http://localhost/subdir/
878842204            http://localhost/
Finished in 16ms
```

# Please note
It sends a HEAD request for each file listed, and I haven't implemented any throttling yet, so it might be a bit of a denial-of-service machine. Use responsibly.

# Installation
 - [Install cargo](https://crates.io/install)
 - clone or download this repository
 - from inside the repository, run `cargo install --path .`
