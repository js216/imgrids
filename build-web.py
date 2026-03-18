import subprocess, pathlib, shutil, sys

if not shutil.which("emcc"):
    sys.exit("emcc not found in PATH.\nRun: source /path/to/emsdk/emsdk_env.sh")

subprocess.run([ "cargo", "build-web"], check=True)

path = "target/wasm32-unknown-emscripten/release/examples"
js  = pathlib.Path(path + "/demo.js").read_text()
html = f"""<!DOCTYPE html><html>
<head><meta charset="utf-8">
<style>
html, body {{
    margin: 0;
    background: #000;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    gap: 0;
}}
#stdout {{
    width: 800px;
    height: 200px;
    background: #111;
    color: #ccc;
    font-family: monospace;
    font-size: 13px;
    overflow-y: auto;
    padding: 6px 8px;
    box-sizing: border-box;
    white-space: pre-wrap;
    word-break: break-all;
}}
</style>
</head>
<body>
<canvas id="canvas" width="800" height="480"></canvas>
<div id="stdout"></div>
<script>
var canvas = document.getElementById('canvas');
var stdout = document.getElementById('stdout');
var Module = {{
    print: function(text) {{
        console.log(text);
        stdout.textContent += text + '\\n';
        stdout.scrollTop = stdout.scrollHeight;
    }},
    printErr: function(text) {{ console.error(text); }},
    canvas: canvas,
}};
window.addEventListener("click", () => window.focus());
</script>
<script>
{js}
</script>
</body></html>"""
pathlib.Path(path + "/index.html").write_text(html)
