// Thumb: Quick Look's picture of one file, when Electron asks ({cmd:'thumb',
// id, path, out, px}: an artifact card's picture, docs/plugin-apps.md A6b,
// architect m_60495). A thumbnail only, never the type's icon: Quick Look
// says itself which files it can draw, so nothing here knows a type. Its
// length too (a video's, a sound's) when AVFoundation reads one. Main gives
// the path (the store's copy or the root's file, never the renderer's), keeps
// one at a time and stops waiting at 5 s; the PNG at `out` is main's to keep.

import AppKit
import AVFoundation
import ImageIO
import QuickLookThumbnailing
import UniformTypeIdentifiers

@MainActor
enum Thumb {
    // the PNG of `path`'s thumbnail at `out`, `px` wide at most: true when written
    static func picture(of path: String, into out: String, px: Int) async -> Bool {
        let url = URL(fileURLWithPath: path)
        let side = CGFloat(px)
        let req = QLThumbnailGenerator.Request(fileAt: url, size: CGSize(width: side, height: side),
                                               scale: 1, representationTypes: .thumbnail)
        do {
            let rep = try await QLThumbnailGenerator.shared.generateBestRepresentation(for: req)
            try? FileManager.default.createDirectory(atPath: (out as NSString).deletingLastPathComponent,
                                                     withIntermediateDirectories: true)
            guard let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: out) as CFURL,
                                                             UTType.png.identifier as CFString, 1, nil)
            else { return false }
            CGImageDestinationAddImage(dest, rep.cgImage, nil)
            return CGImageDestinationFinalize(dest)
        } catch {
            // no thumbnail: Quick Look can't draw this file (the window asks its app)
            return false
        }
    }

    // its length in ms, when AVFoundation reads one (a video, a sound), else nil
    static func length(of path: String) async -> Int? {
        let asset = AVURLAsset(url: URL(fileURLWithPath: path))
        guard let playable = try? await asset.load(.isPlayable), playable,
              let d = try? await asset.load(.duration), d.isNumeric, d.seconds > 0
        else { return nil }
        return Int((d.seconds * 1000).rounded())
    }
}
