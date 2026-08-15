class Ypm < Formula
  desc "Terminal client for YesPlayMusic"
  homepage "https://github.com/nagi-studio/YesPlayMusic"
  license "GPL-3.0-only"

  # Release checklist: replace 0.0.0 here and in both URLs with the tag version
  # (without the leading v), then replace each all-zero SHA-256 with the digest
  # of its corresponding release artifact.
  version "0.0.0"

  on_macos do
    url "https://github.com/nagi-studio/YesPlayMusic/releases/download/v0.0.0/ypm-macos-aarch64",
        using: :nounzip
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"

    depends_on arch: :arm64
  end

  on_linux do
    url "https://github.com/nagi-studio/YesPlayMusic/releases/download/v0.0.0/ypm-linux-x64",
        using: :nounzip
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"

    depends_on arch: :x86_64
    depends_on "alsa-lib"
  end

  def install
    artifact = if OS.mac?
      "ypm-macos-aarch64"
    else
      "ypm-linux-x64"
    end

    bin.install artifact => "ypm"
  end

  test do
    system bin/"ypm", "--version"
  end
end
