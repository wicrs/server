# Contributing to WICRS Server

Thank you for your interest in contributing to WICRS Server! All contributors are expected to abide by the [Code of Conduct](https://github.com/wicrs/server/blob/master/CODE_OF_CONDUCT.md).

## License

WICRS Server is licensed under the [GNU General Public License v3.0](https://github.com/wicrs/server/blob/master/LICENSE).

## Pull Requests

To make changes to WICRS, please send in pull requests on GitHub to the `master`
branch. We'll review them and either merge or request changes. GitHub Actions tests
everything and verifies styling, GitHub Actions must be passing for a pull request
to be merged, you can run `cargo fmt`, `cargo test` and `cargo clippy` locally to 
check if a pull request will pass the GitHub Actions tests.

If you make additions or other changes to a pull request, feel free to either amend
previous commits or only add new ones, however you prefer. We may squash your commits
before merging, depending what we think is best.

## Issue Tracker

You can find the issue tracker [on
GitHub](https://github.com/wicrs/server/issues). If you've found a
problem with WICRS Server, please open an issue there.

We use the following labels:

* `enhancement`: This is for any request for new features or functionality.
* `bug`: This is for anything that's in WICRS Server, but incorrect or not working.
* `discussion`: A discussion about improving something in WICRS Server; this may lead to new
  enhancement or bug issues.

## Development workflow

To build WICRS Server, [install Rust](http://rust-lang.org/install.html), and then:

```bash
git clone https://github.com/wicrs/server wicrs_server
cd wicrs_server
cargo build
```

To run the tests:

```bash
cargo test
```
