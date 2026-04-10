class Brau < Formula
  desc "Fuzzy Homebrew search and install CLI for formulae and casks"
  homepage "https://github.com/shamsghi/brau-cli"
  url "https://github.com/shamsghi/brau-cli/archive/refs/tags/v2.2.6.tar.gz"
  sha256 "f960477ca59198d36ccb3e9ff942161e948a0f64dfce04412f06c60bb0ec6f5d"
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
