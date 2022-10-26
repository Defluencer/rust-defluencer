# Defluencer CLI

IPFS daemon must be running before using the CLI.
 - Command: ```ipfs daemon --enable-pubsub-experiment --enable-namesys-pubsub```

For more info on the commands available to you.
 - Command: ```defluencer --help```

## How To

### Identity Creation
- Create a new identity. Command: ```defluencer node identity --help```

### Channel Creation
- Create a new channel. Command: ```defluencer channel create --help```

### Video Live Streaming
- Start IPFS with PubSub enabled. Command: ```ipfs daemon --enable-pubsub-experiment```
- Start in live streaming mode. Command: ```defluencer stream```
- Run ```ffmpeg_live.sh``` or custom ffmpeg script.
- With your broadcast software output set to ffmpeg. Default: ```rtmp://localhost:2525```
- Start Streaming!
- When done streaming stop your broadcast software.
- Press Ctrl-c to generate timecode CID.
- Use the CLI to create video metadata. Command: ```defluencer user video --help``` for more info.

### Pre-recorded Video
- Start IPFS. Command: ```ipfs daemon```
- Start in file mode. Command: ```defluencer file```
- Run ```ffmpeg_file.sh``` or custom ffmpeg script.
- Wait until the video is processed.
- Press Ctrl-c to generate timecode Cid.
- Use the CLI to create video metadata. Command: ```defluencer user video --help``` for more info.

## Technical

### Requirements
- [IPFS](https://docs.ipfs.tech/install/command-line/#official-distributions)
- [FFMPEG](https://ffmpeg.org/)
- Broadcasting software

### FFMPEG
- Output must be HLS.
- Must use fragmented mp4. (fmp4)
- Media segments length must be 1 second.
- Each track and folder must be named like so. "TRACK_NAME/SEGMENT_INDEX.m4s". egg ```1080p60/24.m4s```
- Audio track must standalone and be named "audio".
- Must produce a master playlist containing all tracks.

Refer to my scripts for inspiration in creating your own.

Due to a bug in FFMPEG, original videos cannot be in .mkv containers, missing metadata will cause missing tracks in HLS master playlist.
In the future, you will be allowed to manually specify codecs and tracks names, that way any video standard could be used.

Keep in mind that web browser support a limited set of codecs.

## License
Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.