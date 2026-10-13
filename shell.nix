{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell {
  # cmake builds libopus from source for the `opus` crate.
  nativeBuildInputs = with pkgs; [ stdenv.cc pkg-config cmake ];
  buildInputs = with pkgs; [ alsa-lib wayland libxkbcommon ];
  # winit, wgpu and cpal's ALSA backend dlopen these at runtime.
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [ wayland libxkbcommon vulkan-loader libGL alsa-lib ]);
  # cpal talks ALSA; on a PipeWire system the default device is PipeWire's ALSA plugin.
  ALSA_PLUGIN_DIR = "${pkgs.pipewire}/lib/alsa-lib";
}
