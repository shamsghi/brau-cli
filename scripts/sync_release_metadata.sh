#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <tag> <sha256>" >&2
  exit 1
fi

tag="$1"
sha256="$2"
version="${tag#v}"

if [[ "$version" == "$tag" ]]; then
  echo "expected a tag like vX.Y.Z, got: $tag" >&2
  exit 1
fi

repo="${GITHUB_REPOSITORY:-shamsghi/brau-cli}"
formula_path="Formula/brau.rb"

perl -0pi -e 's/^version = "\d+\.\d+\.\d+"$/version = "'"$version"'"/m' Cargo.toml
perl -0pi -e 's/(\[\[package\]\]\nname = "brau"\nversion = )"\d+\.\d+\.\d+"/${1}"'"$version"'"/' Cargo.lock
perl -0pi -e 's{^  url ".*"$}{  url "https://github.com/'"$repo"'/archive/refs/tags/'"$tag"'.tar.gz"}m' "$formula_path"
perl -0pi -e 's/^  sha256 ".*"$/  sha256 "'"$sha256"'"/m' "$formula_path"
