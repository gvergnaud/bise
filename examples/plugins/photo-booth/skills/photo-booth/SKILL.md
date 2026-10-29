---
name: photo-booth
description: Open a camera photo booth for the user. Use when the user wants to take a picture, a selfie, a profile photo, or show you something through their camera.
---

# Photo booth

1. Call `tools.photo_booth.open_booth` from run_typescript (`filter`:
   none, mono, sepia, vivid, cool, noir or pop, to start with one).
2. The user takes photos in the view under your call, then copies,
   saves or sends one. A sent photo arrives as a message: "Here is my
   photo: <path>" (a PNG at that path). Read it with your image tool when
   you need to see it.
3. Answer in one line; the booth is the interface.
