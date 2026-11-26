package app.assistant

import android.app.PendingIntent
import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService

/**
 * Quick settings tiles (pull down the top of the screen, edit, drag them in).
 * Talk opens the voice card, even on the lock screen. New chat opens the app on a fresh chat with the keyboard up.
 */
abstract class OpenTile : TileService() {
    abstract fun target(): Intent
    /** True when the screen it opens may show over the lock screen. */
    open val overLock = false

    override fun onStartListening() {
        super.onStartListening()
        qsTile?.apply { state = Tile.STATE_INACTIVE; updateTile() }
    }

    // Android 14+ uses the PendingIntent form below; the Intent form is only for Android 12 and 13.
    @android.annotation.SuppressLint("StartActivityAndCollapseDeprecated")
    override fun onClick() {
        super.onClick()
        val go = {
            val i = target().addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            if (Build.VERSION.SDK_INT >= 34) {
                startActivityAndCollapse(PendingIntent.getActivity(this, i.component.hashCode(), i, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT))
            } else {
                @Suppress("DEPRECATION")
                startActivityAndCollapse(i)
            }
        }
        if (isLocked && !overLock) unlockAndRun { go() } else go()
    }
}

class TalkTile : OpenTile() {
    override val overLock = true
    override fun target() = Intent(this, AssistActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
}

class ChatTile : OpenTile() {
    override fun target() = Intent(this, MainActivity::class.java)
        .setAction("app.assistant.OPEN")
        .putExtra(MainActivity.EXTRA_OPEN, "new-chat")
}
