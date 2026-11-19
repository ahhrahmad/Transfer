// SPDX-License-Identifier: MIT
package com.transfer.fork.ui

import android.content.Context
import android.util.AttributeSet
import androidx.preference.Preference
import androidx.preference.PreferenceViewHolder
import com.google.android.material.materialswitch.MaterialSwitch
import com.transfer.fork.R
import com.transfer.fork.util.Constants

/**
 * Preference row rendered with Material 3 `MaterialSwitch` (looks like the system
 * toggle on A13+). SwitchPreferenceCompat renders the legacy SwitchCompat and has no
 * officially supported way to swap the widget in XML.
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

        val sw = holder.findViewById(R.id.material_switch) as MaterialSwitch
        sw.isChecked = checked
        sw.setOnCheckedChangeListener { _, b ->
            checked = b
            prefs.edit().putBoolean(key, b).apply()
        }
        holder.itemView.setOnClickListener { sw.toggle() }
    }
}
