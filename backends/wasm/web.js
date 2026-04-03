mergeInto(LibraryManager.library, {
    // Shared event queue - declared inside mergeInto so Emscripten includes
    // the initializer in the generated JS output. Code outside mergeInto runs
    // only at link time, not at runtime.
    $imgrids_queue: [],

    imgrids_blit: function(ptr, byte_len, width, height) {
        var ctx = Module.canvas.getContext('2d');
        var data = new Uint8ClampedArray(HEAPU8.buffer, ptr, byte_len);
        ctx.putImageData(new ImageData(data, width, height), 0, 0);
    },

    imgrids_setup_input__deps: ['$imgrids_queue'],
    imgrids_setup_input: function() {
        var canvas = Module.canvas;

        function push(type, clientX, clientY) {
            var rect = canvas.getBoundingClientRect();
            imgrids_queue.push({
                type: type,
                x: Math.floor((clientX - rect.left) * (canvas.width  / rect.width)),
                y: Math.floor((clientY - rect.top)  * (canvas.height / rect.height)),
            });
        }

        canvas.addEventListener('mousedown', function(e) {
            if (e.button === 0) push(0, e.clientX, e.clientY);
        });
        canvas.addEventListener('mouseup', function(e) {
            if (e.button === 0) push(1, e.clientX, e.clientY);
        });
        canvas.addEventListener('mousemove', function(e) {
            if (e.buttons & 1) push(2, e.clientX, e.clientY);
        });
        canvas.addEventListener('touchstart', function(e) {
            e.preventDefault();
            var t = e.changedTouches[0];
            push(0, t.clientX, t.clientY);
        }, { passive: false });
        canvas.addEventListener('touchend', function(e) {
            e.preventDefault();
            var t = e.changedTouches[0];
            push(1, t.clientX, t.clientY);
        }, { passive: false });
        canvas.addEventListener('touchmove', function(e) {
            e.preventDefault();
            var t = e.changedTouches[0];
            push(2, t.clientX, t.clientY);
        }, { passive: false });
    },

    imgrids_next_event__deps: ['$imgrids_queue'],
    imgrids_next_event: function(out_type, out_x, out_y) {
        if (imgrids_queue.length === 0) return 0;
        var ev = imgrids_queue.shift();
        HEAP32[out_type >> 2] = ev.type;
        HEAP32[out_x   >> 2] = ev.x;
        HEAP32[out_y   >> 2] = ev.y;
        return 1;
    },
});
