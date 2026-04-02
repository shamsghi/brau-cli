class Brau < Formula
  desc "Fuzzy Homebrew search and install CLI for formulae and casks"
  homepage "https://github.com/shamsghi/brau-cli"
  url "https://github.com/shamsghi/brau-cli.git",
      tag:      "v2.2.0",
      revision: "56b1f56b9d997ef786c7f892eb9a730c385a1a78"
  license "MIT"
  head "https://github.com/shamsghi/brau-cli.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    output = shell_output("#{bin}/brau search ripgrap --limit 1 --no-anim --no-finale")
    assert_match "ripgrep", output

    help_output = shell_output("#{bin}/brau help search --no-anim 2>&1")
    assert_match "brew search", help_output
  end
end
