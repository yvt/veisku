# veisku

Opinionated, personal file-oriented document manager

This tool operates on a local directory (called a *document root*) containing *documents*. A document root may contain a configuration file `.veisku/config.toml` that controls the default behavior of the tool (see `src/cfg.rs` for the configuration scheme).

This tool recognizes Markdown YAML preambles and provides a search query syntax for their fields.

A document root is located by traversing up from the current directory until it finds one containing a directory named `.veisku`. The current directory will be used if none was found.

The following operations are supported:

 - List documents (`v ls`). Accepts the common search query syntax.

 - Run a command in the document root (`v run`).
 
 - Open the specified document (`v open`) using `open` or `xdg-open`. Accepts the common search query syntax but fails if more than one document matches.

 - Show the specified document (`v show`) using `$PAGER` or `less`. Accepts the common search query syntax but fails if more than one document matches.

 - Edit the specified document (`v edit`) using `$EDITOR`. Accepts the common search query syntax but fails if more than one document matches.

 - Display the path of the specified document (`v which`). Accepts the common search query syntax but fails if more than one document matches.
