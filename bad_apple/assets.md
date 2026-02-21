# Requirements

This script requires **raw video asset files** in order to run properly.

Before using the script, you must provide it with the necessary assets — specifically `.raw` video files in **monochrome (monob)** format.

---

## Generate Assets Using `ffmpeg`

You can create the required `.raw` assets from an existing video file (e.g., `.mp4`) with the following command:

```bash
ffmpeg -i ./<path>/<to>/<input_file>.mp4 \
  -vf "format=monob" \
  -an \
  -f rawvideo \
  -pix_fmt monob \
  ./<path>/<to>/<output_file>.raw
```

## Optional: Scale While Converting

If you want to **scale the video** while converting, include a `scale` filter inside the video filter:

```bash
ffmpeg -i ./<path>/<to>/<input_file>.mp4 \
  -vf "scale=640:480,format=monob" \
  -an \
  -f rawvideo \
  -pix_fmt monob \
  ./<path>/<to>/<output_file>.raw
```

Replace `640:480` with the desired width and height.

---

## Summary

To run this script successfully:

1. Provide one or more `.raw` asset files in **monochrome format**.
2. If you don't already have them, use the `ffmpeg` commands above to generate them.
