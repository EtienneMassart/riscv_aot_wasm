# mdBook source

This directory is ready for the [mdBook](https://rust-lang.github.io/mdBook/) generator.

```sh
mdbook serve --open
```

To create a static site:

```sh
mdbook build
```

The generated site is written to `book/`. Source PDF page markers are retained as HTML comments for traceability.
