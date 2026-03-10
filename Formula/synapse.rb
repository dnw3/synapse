class Synapse < Formula
  desc "AI Agent powered by Synaptic framework"
  homepage "https://github.com/dnw3/synapse"
  url "https://github.com/dnw3/synapse/archive/refs/tags/v0.2.0.tar.gz"
  sha256 "PLACEHOLDER_SHA256"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--root", prefix, "--path", ".", "--features", "full"
  end

  test do
    assert_match "synapse", shell_output("#{bin}/synapse --version")
  end
end
