package org.zioncode.app

internal object NativeBridge {
    init {
        System.loadLibrary("zion_android")
    }

    external fun decodeBookPng(
        pngBytes: ByteArray,
        passphraseBytes: ByteArray,
    ): ByteArray
}
