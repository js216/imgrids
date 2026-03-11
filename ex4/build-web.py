import subprocess, pathlib, shutil, sys

if not shutil.which("emcc"):
    sys.exit("emcc not found in PATH.\nRun: source /path/to/emsdk/emsdk_env.sh")

subprocess.run([ "cargo", "build-web"], check=True)

path = "target/wasm32-unknown-emscripten/release/examples"
js  = pathlib.Path(path + "/demo.js").read_text()
html = f"""<!DOCTYPE html><html>
<head><meta charset="utf-8">
<style>html,body{{margin:0;background:#000;display:flex;align-items:center;justify-content:center}}</style>
</head>
<body>
<canvas id="canvas" width="800" height="480"></canvas>
<script>
var canvas = document.getElementById('canvas');
var Module = {{
    print:    function(text) {{ console.log(text); }},
    printErr: function(text) {{ console.error(text); }},
    canvas:   canvas,
}};
window.addEventListener("click", () => window.focus());
</script>
<script>
{js}
</script>
</body></html>"""
pathlib.Path(path + "/index.html").write_text(html)
