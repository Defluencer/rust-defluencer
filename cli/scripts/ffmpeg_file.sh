#!/bin/bash
# BASH script example.

# FFMPEG configured to output multi quality HLS with standalone audio track.

# Tips:
# GoP must be twice the segment length in frames then keyint_min must be segment length in frames.
# Scene change is active and insert B/P frames.
# Then force I frames at interval segment length in frames.

# Input MUST be 60fps and audio will be resampled to ensure browser codec support.

echo -e "Where is the video file you would like to process?" 
read file

ffmpeg -i $file \
-filter_complex \
"[0:v]split=3[1080p60][in1][in2]; \
[in1]scale=w=1280:h=720,split=2[720p60][scaleout]; \
[scaleout]fps=30[720p30]; \
[in2]fps=30,scale=w=854:h=480[480p30]" \
-map '[1080p60]' -c:v:0 libx264 -preset: fast -rc-lookahead:0 60 -g:0 120 -keyint_min:0 60 -force_key_frames:0 "expr:eq(mod(n,60),0)" -b:v:0 12000k \
-map '[720p60]' -c:v:1 libx264 -rc-lookahead:1 60 -g:1 120 -keyint_min:1 60 -force_key_frames:1 "expr:eq(mod(n,60),0)" -b:v:1 7500k \
-map '[720p30]' -c:v:2 libx264 -rc-lookahead:2 30 -g:2 60 -keyint_min:2 30 -force_key_frames:2 "expr:eq(mod(n,30),0)" -b:v:2 5000k \
-map '[480p30]' -c:v:3 libx264 -rc-lookahead:3 30 -g:3 60 -keyint_min:3 30 -force_key_frames:3 "expr:eq(mod(n,30),0)" -b:v:3 2500k \
-map a:0 -c:a:0 aac -b:a:0 128k \
-f hls -var_stream_map "v:0,name:1080p60 v:1,name:720p60 v:2,name:720p30 v:3,name:480p30 a:0,name:audio" \
-hls_init_time 1 -hls_time 1 -hls_flags independent_segments -master_pl_name master.m3u8 \
-hls_segment_type fmp4 -hls_segment_filename http://localhost:2526/%v/%d.m4s \
-http_persistent 0 -ignore_io_errors 1 -method PUT http://localhost:2526/%v/index.m3u8