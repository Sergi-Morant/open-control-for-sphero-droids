package com.sergio.opencontrol

import android.os.Bundle
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent

class MainActivity : ComponentActivity() {
    private val app: AppState get() = state ?: AppState(applicationContext).also { state = it }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Mientras se controla un droide, la pantalla no se apaga sola.
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        val app = app
        setContent { AppUi(app) }
    }

    override fun dispatchGenericMotionEvent(e: MotionEvent): Boolean = app.pad.onMotion(e) || super.dispatchGenericMotionEvent(e)

    override fun dispatchKeyEvent(e: KeyEvent): Boolean = app.pad.onKey(e) || super.dispatchKeyEvent(e)

    companion object {
        /// El estado sobrevive a que Android recree la pantalla; las conexiones viven con el proceso.
        private var state: AppState? = null
    }
}
