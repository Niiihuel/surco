{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell {
  # cmake builds libopus from source for the `opus` crate.
  # pactl is how the engine lists the sound server's outputs and moves its stream to one.
  nativeBuildInputs = with pkgs; [ stdenv.cc pkg-config cmake pulseaudio ];
  buildInputs = with pkgs; [ alsa-lib wayland libxkbcommon ];
  # winit, wgpu and cpal's ALSA backend dlopen these at runtime.
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [ wayland libxkbcommon vulkan-loader libGL alsa-lib ]);
  # The H.264 decoder is C++. The player is told where its standard library is when it is
  # linked (see crates/app/build.rs): on the library path it would also reach every program
  # started from this shell, and the system's own are built against a newer one.
  DNM_CXX_RUNTIME = "${pkgs.stdenv.cc.cc.lib}/lib";
  # cpal talks ALSA; on a PipeWire system the default device is PipeWire's ALSA plugin.
  ALSA_PLUGIN_DIR = "${pkgs.pipewire}/lib/alsa-lib";
}
