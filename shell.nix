{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell {
  # cmake builds libopus from source for the `opus` crate.
  # pactl is how the engine lists the sound server's outputs and moves its stream to one.
  nativeBuildInputs = with pkgs; [ stdenv.cc pkg-config cmake pulseaudio ];
  buildInputs = with pkgs; [ alsa-lib wayland libxkbcommon ];
  # winit, wgpu and cpal's ALSA backend dlopen these at runtime, and the H.264 decoder is
  # C++ that wants its standard library.
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [ wayland libxkbcommon vulkan-loader libGL alsa-lib stdenv.cc.cc.lib ]);
  # cpal talks ALSA; on a PipeWire system the default device is PipeWire's ALSA plugin.
  ALSA_PLUGIN_DIR = "${pkgs.pipewire}/lib/alsa-lib";
}
