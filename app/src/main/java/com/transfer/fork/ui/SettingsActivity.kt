// SPDX-License-Identifier: MIT
package com.transfer.fork.ui

import android.net.Uri
import android.os.Bundle
import android.view.MenuItem
import android.widget.Toast
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.Toolbar
import androidx.core.content.edit
import androidx.core.net.toUri

import androidx.preference.EditTextPreference
import androidx.preference.Preference
import androidx.preference.PreferenceFragmentCompat
import com.transfer.fork.util.Constants
import com.transfer.fork.util.FileUtils
import com.transfer.fork.R


class SettingsActivity : AppCompatActivity() {

    private var settingsFragment: SettingsFragment? = null

    private val selectFolderLauncher =
        registerForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri: Uri? ->
            if (uri != null) {
                FileUtils.persistUriPermission(this, uri)
                val prefs = getSharedPreferences(Constants.SHARED_PREFS_NAME, MODE_PRIVATE)
                prefs.edit { putString(Constants.EXTRA_FOLDER_URI, uri.toString()) }
                settingsFragment?.updateFolderSummary()
                Toast.makeText(this, "Shared folder selected", Toast.LENGTH_SHORT).show()
            }
        }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        if (item.itemId == android.R.id.home) {
            finish()
        }
        return super.onOptionsItemSelected(item)
    }


    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_settings)
        val toolbar = findViewById<Toolbar>(R.id.toolbar)
        setSupportActionBar(toolbar)
        if (savedInstanceState == null) {
            val fragment = SettingsFragment()
            settingsFragment = fragment
            supportFragmentManager
                .beginTransaction()
                .replace(R.id.settings_container, fragment)
                .commit()
        } else {
            settingsFragment = supportFragmentManager.findFragmentById(R.id.settings_container) as? SettingsFragment
        }
        supportActionBar?.setDisplayHomeAsUpEnabled(true)

        supportActionBar?.title = getString(R.string.title_activity_settings)
    }

    fun launchFolderSelection() {
        selectFolderLauncher.launch(null)
    }

    class SettingsFragment : PreferenceFragmentCompat() {
        override fun onCreatePreferences(savedInstanceState: Bundle?, rootKey: String?) {
            // Tell PreferenceManager to use the right SharedPreferences file (see #18)
            preferenceManager.sharedPreferencesName = Constants.SHARED_PREFS_NAME
            setPreferencesFromResource(R.xml.preferences, rootKey)

            // Password Preference
            val passwordPreference =
                findPreference<EditTextPreference>(getString(R.string.pref_key_server_password))
            updatePasswordSummary(passwordPreference)
            passwordPreference?.setOnPreferenceChangeListener { preference, newValue ->
                val newPassword = newValue as String?
                if (newPassword.isNullOrEmpty()) {
                    preference.summary = getString(R.string.pref_summary_password_protect_off)
                    Toast.makeText(
                        requireContext(),
                        getString(R.string.password_cleared),
                        Toast.LENGTH_SHORT
                    ).show()
                } else {
                    preference.summary = getString(R.string.pref_summary_password_protect_on)
                    Toast.makeText(
                        requireContext(),
                        getString(R.string.password_set),
                        Toast.LENGTH_SHORT
                    ).show()
                }
                true
            }

            // Folder Selection Preference
            val folderPreference = findPreference<Preference>("pref_key_shared_folder")
            folderPreference?.setOnPreferenceClickListener {
                (activity as? SettingsActivity)?.launchFolderSelection()
                true
            }
            updateFolderSummary()
        }

        private fun updatePasswordSummary(passwordPreference: EditTextPreference?) {
            val prefs = preferenceManager.sharedPreferences


            val password = prefs
                ?.getString(getString(R.string.pref_key_server_password), null)
            if (password.isNullOrEmpty()) {
                passwordPreference?.summary = getString(R.string.pref_summary_password_protect_off)
            } else {
                passwordPreference?.summary = getString(R.string.pref_summary_password_protect_on)
            }
        }

        fun updateFolderSummary() {
            val prefs = preferenceManager.sharedPreferences
            val uriString = prefs?.getString(Constants.EXTRA_FOLDER_URI, null)
            val folderPreference = findPreference<Preference>("pref_key_shared_folder")
            if (uriString != null) {
                val uri = uriString.toUri()
                // Avoid I/O on the main thread (DocumentFile.fromTreeUri can block on some
                // providers/ROMs and freeze Settings). Show the last path segment instead.
                val raw = uri.lastPathSegment ?: uri.path ?: ""
                // Capitalize the storage volume name (e.g. "primary:" -> "Primary:").
                val summary = if (raw.contains(":")) {
                    val idx = raw.indexOf(":")
                    raw.substring(0, idx).replaceFirstChar { it.uppercaseChar() } + ": " + raw.substring(idx + 1).trimStart()
                } else raw
                folderPreference?.summary = summary
            } else {
                folderPreference?.summary = getString(R.string.no_folder_selected)
            }
        }
    }

    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        delegate.applyDayNight()
    }

    override fun onSupportNavigateUp(): Boolean {
        onBackPressedDispatcher.onBackPressed()
        return true
    }
}