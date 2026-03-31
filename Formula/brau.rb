class Brau < Formula
  desc "Fuzzy Homebrew search and install CLI for formulae and casks"
  homepage "https://github.com/pancake/brau"
  url "https://github.com/pancake/brau.git",
      branch: "main"
  version "main"
  head "https://github.com/pancake/brau.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    output = shell_output("#{bin}/brau ripgrap --limit 1")
    assert_match "ripgrep", output
  end
end
