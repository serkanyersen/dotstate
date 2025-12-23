class Dotstate < Formula
  desc "A modern, secure, and user-friendly dotfile manager built with Rust"
  homepage "https://github.com/serkanyersen/dotstate"
  url "https://codeload.github.com/serkanyersen/dotstate/tar.gz/v0.1.1"
  sha256 "PLACEHOLDER_SHA256"
  license "MIT"
  head "https://github.com/serkanyersen/dotstate.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--path", ".", "--root", prefix, "--locked"
  end

  test do
    assert_match "dotstate", shell_output("#{bin}/dotstate --version")
  end
end
