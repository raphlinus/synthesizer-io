# Synthesizer IO

Hopefully, this will become a synthesizer written in Rust. At the moment, it's
experimental bits of technology toward that end.

## Running

At the moment, the main repo contains a very primitive command line app for a simple
mono subtractive synth. A MIDI keyboard is required, and the mapping for controls is
set up only for an Akai MPK mini. In addition, settings for MIDI and sound are
somewhat hardwired; it works on my Windows and Mac systems, but is likely fragile.

There is also a web demo in the synthesizer-io-wasm directory. All it does is play a
sawtooth, but it shows that it's possible to compile to wasm and run the synth engine
in a browser.
