# xerrno

[![Crates.io](https://img.shields.io/crates/v/xerrno)](https://crates.io/crates/xerrno)
[![Docs.rs](https://docs.rs/xerrno/badge.svg)](https://docs.rs/xerrno)
[![CI](https://github.com/arceos-org/axerrno/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/arceos-org/axerrno/actions/workflows/ci.yml)

Generic error code representation.

It provides two error types and the corresponding result types:

- [`XError`] and [`XResult`]: A generic error type similar to
[`std::io::ErrorKind`].
- [`LinuxError`] and [`LinuxResult`]: Linux specific error codes defined in
`errno.h`. It can be converted from [`XError`].

[`XError`]: https://docs.rs/xerrno/latest/xerrno/enum.XError.html
[`XResult`]: https://docs.rs/xerrno/latest/xerrno/type.XResult.html
[`LinuxError`]: https://docs.rs/xerrno/latest/xerrno/enum.LinuxError.html
[`LinuxResult`]: https://docs.rs/xerrno/latest/xerrno/type.LinuxResult.html
[`std::io::ErrorKind`]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html
