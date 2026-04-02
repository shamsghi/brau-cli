class Brau < Formula
  desc "Fuzzy Homebrew search and install CLI for formulae and casks"
  homepage "https://github.com/shamsghi/brau-cli"
  url "https://github.com/shamsghi/brau-cli/archive/refs/tags/v2.2.0.tar.gz"
  sha256 "d3688cbc8b777c8a94140fa40157e687ad2037be42d1386c5509965e653f0e5b"
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
