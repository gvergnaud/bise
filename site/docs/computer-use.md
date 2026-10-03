---
title: computer use
description: agents that open pages in Chrome and click and type in your Mac apps, in the background, with your logins. off until you turn it on.
---

computer use lets an agent see and act in a web page or an app: open a page, read it, click, type, fill a form, take a screenshot. it works in Chrome, in background tabs of the agent's own tab group, with your logins. with Accessibility allowed, it also drives your Mac apps (Notes, TextEdit, Figma, ...). it never takes your screen or your active tab.

## turn it on

computer use is off until you turn it on. off, it costs nothing: no tools, no process, no file.

```text
/computer-use
```

this turns it on and opens its setup, one step at a time. bise checks each step by itself while you do it:

1. **Chrome**: open, and recent enough.
2. **the extension**: bise's Chrome extension, loaded in `chrome://extensions`. bise copies the folder's path for you.
3. **a test**: bise opens a tab in the background and clicks a button.
4. **apps**, optional: turn on bise Computer Use in System Settings → Privacy & Security → Accessibility. Screen Recording is optional too, for screenshots of apps.

then ask any agent to use Chrome or an app. the agents that are already open get computer use at their next idle.

## what happens when an agent acts

> careful: in yolo, agents act without asking, purchases included. `shift+tab` switches to auto, where agents ask you in their thread before buying, sending or posting. in both modes, they never type passwords: when a site asks you to log in, the agent asks you to do it.

- what the agents see (page text, screenshots) goes only to the model of the agent that took it. the setup screen names that provider.
- some places are off limits in every mode: Chrome's own pages (`chrome://`, settings), the extension stores, and apps like password managers and your terminal. the agent asks you to do that step.
- `/stop <agent>` stops an agent's turn and its hands on Chrome or an app, until you write to it again. Chrome's "bise started debugging this browser" bar has a Cancel button that does the same.

## turn it off

```text
/computer-use off         # agents lose it at their next idle
/computer-use uninstall   # also removes the browser hosts and bise's computer-use files
```

after an uninstall, two things are yours to do: remove the extension in Chrome, and remove bise Computer Use from System Settings → Privacy & Security (Accessibility, Screen Recording).

## limits

- Chrome only for now. Safari and Firefox are not supported.
- macOS only.
- pages bise can't read: `chrome://` pages, the Chrome Web Store, other extensions' pages, the text inside the PDF viewer.
