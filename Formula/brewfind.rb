class Brewfind < Formula
  desc "Fuzzy Homebrew search and install CLI for formulae and casks"
  homepage "https://github.com/pancake/brewfind"
  url "https://github.com/pancake/brewfind.git",
      branch: "main"
  version "main"
  head "https://github.com/pancake/brewfind.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    output = shell_output("#{bin}/brewfind ripgrap --limit 1")
    assert_match "ripgrep", output
  end
end
