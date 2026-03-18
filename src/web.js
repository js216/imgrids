mergeInto(LibraryManager.library, {
    imgrids_blit: function(ptr, byte_len, width, height) {
        var ctx = Module.canvas.getContext('2d');
        var data = new Uint8ClampedArray(HEAPU8.buffer, ptr, byte_len);
        ctx.putImageData(new ImageData(data, width, height), 0, 0);
    }
});
