"""Render kernel-dumped mesh frames into a small looping GIF (task 34).

Usage: python render_gif.py <framesdir> <out.gif>

Frames come from the kernel's own worker_mesh tessellation (see
crates/keel-topo/examples/gif_frames.rs). Rendering is a tiny painter's
software rasterizer over matplotlib polygons: flat shading from the
per-triangle normals, fixed isometric-ish camera, feature lines drawn
from the kernel's edge polylines. Target: 250x250, well under 1.5 MB.
"""

import json
import sys
from pathlib import Path

import numpy as np
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.collections import PolyCollection, LineCollection
import imageio.v2 as imageio

# Fixed camera: rotate about z by AZ, tilt by EL, orthographic.
AZ = np.deg2rad(-35.0)
EL = np.deg2rad(28.0)
LIGHT = np.array([0.45, -0.35, 0.82])
LIGHT = LIGHT / np.linalg.norm(LIGHT)
BASE = np.array([0.22, 0.45, 0.65])  # keel blue
BG = "#0d1117"  # GitHub dark


def camera():
    ca, sa = np.cos(AZ), np.sin(AZ)
    ce, se = np.cos(EL), np.sin(EL)
    rz = np.array([[ca, -sa, 0], [sa, ca, 0], [0, 0, 1]])
    rx = np.array([[1, 0, 0], [0, ce, -se], [0, se, ce]])
    return rx @ rz


def render_frame(data, bounds, size=250):
    cam = camera()
    pos = np.array(data["positions"], dtype=np.float64).reshape(-1, 3)
    nrm = np.array(data["normals"], dtype=np.float64).reshape(-1, 3)
    idx = np.array(data["indices"], dtype=np.int64).reshape(-1, 3)
    tri = pos[idx]  # (n, 3, 3)
    tn = nrm[idx[:, 0]]  # flat shading: per-triangle normal

    view = tri @ cam.T
    # Painter's order: farthest first (camera looks down +y_view).
    depth = view[:, :, 1].mean(axis=1)
    order = np.argsort(-depth)
    view = view[order]
    tn = tn[order]

    # Backface cull (view dir = -y after the rotation).
    vn = tn @ cam.T
    front = vn[:, 1] < 0.0
    view, tn = view[front], tn[front]

    shade = np.clip(tn @ LIGHT, 0.0, 1.0)
    colors = np.clip(BASE[None, :] * (0.30 + 1.15 * shade[:, None]), 0, 1)

    fig = plt.figure(figsize=(size / 100, size / 100), dpi=100)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.set_facecolor(BG)
    fig.set_facecolor(BG)
    # Edge color = face color, slim width: hides the hairline AA seams
    # between adjacent triangles without drawing a visible wireframe.
    polys = PolyCollection(
        view[:, :, [0, 2]],
        facecolors=colors,
        edgecolors=colors,
        linewidths=0.35,
        antialiaseds=True,
    )
    ax.add_collection(polys)

    lines = np.array(data.get("lines", []), dtype=np.float64).reshape(-1, 2, 3)
    if len(lines):
        lv = lines @ cam.T
        ax.add_collection(
            LineCollection(lv[:, :, [0, 2]], colors="#dce6f0", linewidths=0.8, alpha=0.45)
        )

    (x0, x1), (z0, z1) = bounds
    ax.set_xlim(x0, x1)
    ax.set_ylim(z0, z1)
    ax.set_aspect("equal")
    ax.axis("off")

    meta = data.get("meta", {})
    if "points" in meta:
        pts = np.array(meta["points"], dtype=np.float64).reshape(-1, 3) @ cam.T
        wn = np.array(meta["winding"], dtype=np.float64)
        inside = wn > 0.5
        ax.scatter(
            pts[~inside, 0], pts[~inside, 2], s=4, c="#4d5866", alpha=0.65, zorder=5
        )
        ax.scatter(
            pts[inside, 0], pts[inside, 2], s=7, c="#56d364", alpha=0.95, zorder=6
        )
    if meta.get("label"):
        ax.text(
            0.05, 0.05, meta["label"], transform=ax.transAxes,
            color="#dce6f0", fontsize=9, family="monospace",
        )
    if "gap" in meta:
        ax.text(
            0.05, 0.05, f"gap {meta['gap']:.0e}", transform=ax.transAxes,
            color="#8b98a8", fontsize=8, family="monospace",
        )
    if meta.get("declined"):
        ax.text(
            0.05, 0.05, "DECLINED (never a wrong body)",
            transform=ax.transAxes, color="#e3b341", fontsize=8,
            family="monospace", weight="bold",
        )
    if meta.get("salvaged"):
        ax.text(
            0.05, 0.05, f"salvaged @ {meta['achieved']:.0e}",
            transform=ax.transAxes, color="#56d364", fontsize=8,
            family="monospace", weight="bold",
        )
    fig.canvas.draw()
    buf = np.asarray(fig.canvas.buffer_rgba())[:, :, :3].copy()
    plt.close(fig)
    return buf


def main():
    frames_dir = Path(sys.argv[1])
    out = Path(sys.argv[2])
    files = sorted(frames_dir.glob("frame_*.json"))
    if not files:
        sys.exit(f"no frames in {frames_dir}")
    datas = [json.loads(f.read_text()) for f in files]

    # Shared bounds across all frames so the camera never jumps.
    cam = camera()
    lo = np.array([np.inf, np.inf])
    hi = -lo
    for d in datas:
        pos = np.array(d["positions"], dtype=np.float64).reshape(-1, 3) @ cam.T
        xz = pos[:, [0, 2]]
        lo = np.minimum(lo, xz.min(axis=0))
        hi = np.maximum(hi, xz.max(axis=0))
    pad = 0.06 * (hi - lo).max()
    bounds = ((lo[0] - pad, hi[0] + pad), (lo[1] - pad, hi[1] + pad))

    images = [render_frame(d, bounds) for d in datas]
    # Hold the final state so the payoff reads before the loop restarts.
    durations = [0.09] * len(images)
    durations[-1] = 1.4
    imageio.mimsave(out, images, format="GIF", duration=durations, loop=0)
    print(f"{out}: {len(images)} frames, {out.stat().st_size / 1024:.0f} KiB")


if __name__ == "__main__":
    main()
