// SPDX-License-Identifier: MIT
package com.transfer.fork.ui

import android.content.Context
import android.util.AttributeSet
import android.widget.ImageView
import androidx.preference.Preference
import androidx.preference.PreferenceViewHolder
import com.transfer.fork.R
import com.transfer.fork.util.Constants

/**
 * Preference row rendered with a Material Symbols toggle icon (toggle_on/toggle_off).
 * Using an ImageView avoids the SwitchCompat.onMeasure NPE seen on some ROMs and
 * keeps the icon visible across Android versions.
 */
class MaterialSwitchPreference(context: Context, attrs: AttributeSet? = null) :
    Preference(context, attrs) {

    private var checked = true

    init {
        layoutResource = R.layout.preference_material_switch
    }

    override fun onBindViewHolder(holder: PreferenceViewHolder) {
        super.onBindViewHolder(holder)
        val prefs = context.getSharedPreferences(Constants.SHARED_PREFS_NAME, Context.MODE_PRIVATE)
        checked = if (key == null) true else prefs.getBoolean(key!!, true)

        val toggle = holder.findViewById(R.id.material_switch) as ImageView
        toggle.setImageResource(if (checked) R.drawable.ic_toggle_on else R.drawable.ic_toggle_off)
        toggle.setOnClickListener {
            checked = !checked
            if (key != null) prefs.edit().putBoolean(key, checked).apply()
            toggle.setImageResource(if (checked) R.drawable.ic_toggle_on else R.drawable.ic_toggle_off)
        }
        holder.itemView.setOnClickListener { toggle.performClick() }
    }
}