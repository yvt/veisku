# `veisku`

Opinionated, personal file-oriented document manager

## Operation

This tool operates on a local directory (called a *document root*) containing *documents*. A document root may contain a configuration file `.veisku/config.toml`, which controls the default behavior of the tool (see `src/cfg.rs` for the configuration scheme). A document root is found by traversing up from the current directory until it finds one containing a directory named `.veisku`. The current directory will be used if none was found.

This tool recognizes Markdown YAML preambles and provides a search query syntax for their fields.

The following operations are supported:

 - List documents (`v ls`). Accepts the common search query syntax.

 - Run a command in the document root (`v run`).
 
 - Open the specified document (`v open`) using `open` or `xdg-open`. Accepts the common search query syntax but fails if more than one document matches.

 - Show the specified document (`v show`) using `$PAGER` or `less`. Accepts the common search query syntax but fails if more than one document matches.

 - Edit the specified document (`v edit`) using `$EDITOR`. Accepts the common search query syntax but fails if more than one document matches.

 - Display the path of the specified document (`v which`). Accepts the common search query syntax but fails if more than one document matches.

## Example

```shell
$ v ls tags:personal !tags:blocked
d90ee0b    [personal] Markright: Remove the border from the ToC button
e579f3f    [open] [personal] Make a reservation for a flu vaccine shot
...

$ v show 341e
---
title: "Markright: Remove the border from the ToC button"
tags: [personal]
---
...

$ v run rg flu
e579f3f.md
2:title: "Make a reservation for a flu vaccine shot"

$ v run git push
```

In this example, the document root is organized as follows:

```text
.veisku/
  config.toml
  issues
    d90ee0b.md
    ...
```

`config.toml`:

```toml
# look for documents in `.veisku/issues`
root = ".veisku/issues"
```

`.veisku` could be placed in the home directory so that the `v` command can be used wherever the current working directory is, as long as it's inside the home directory.

## Related Works

 - [`git-issue`]: A minimalist Git-based issue management system. I wrote `veisku` as a more performant, more generic replacement of `git-issue` to suit my use case. `veisku` addresses the following problems with `git-issue`, which I personally encountered while using it as a personal task tracker:

     - *Shell scripts are incredibly slow to execute*. I used my custom fork of `git-issue` to display tags in an issue list, colored based on a JSON configuration. The time taken to render an issue list started to be unbearable as the number of issues burgeoned throughout time.

     - *Automatically creating a commit on every change makes the commit history incomprehensible*. It's not trivial to display the edit history of a particular issue. In comparison, `veisku` is not even aware of Git. A user of `veisku` can execute Git commands by `v run git ...` or use a Git frontend of their choice. They can use the command `v show -c 'git,log,-p' ...` to display the edit history of an entry.

 - Any online task tracker such as Trello and online note app such as Evernote: There are many such tools. They are indeed great for collaboration and inter-device synchronization. For a personal, long-lived record, however, I felt it more right to manage it with physical ownership and an application-agnostic data format so that it doesn't become unreadable at someone's whim.

[`git-issue`]: https://github.com/dspinellis/git-issue
