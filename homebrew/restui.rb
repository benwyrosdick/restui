class Restui < Formula
  desc "A TUI API testing tool like Postman"
  homepage "https://github.com/benwyrosdick/restui"
  version "0.1.2"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/benwyrosdick/restui/releases/download/v#{version}/restui-aarch64-apple-darwin.tar.gz"
      sha256 "SHA256_ARM_DARWIN"
    else
      url "https://github.com/benwyrosdick/restui/releases/download/v#{version}/restui-x86_64-apple-darwin.tar.gz"
      sha256 "SHA256_X86_DARWIN"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      odie "restui is not currently supported on Linux ARM"
    else
      url "https://github.com/benwyrosdick/restui/releases/download/v#{version}/restui-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "SHA256_LINUX"
    end
  end

  def install
    bin.install "restui"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/restui --version")
  end
end
