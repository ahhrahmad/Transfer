document.addEventListener('DOMContentLoaded', () => {
    const dropZone = document.getElementById('drop-zone');
    const fileInput = document.getElementById('file-input');
    const filesTableBody = document.querySelector('#files-table tbody');
    const uploadProgressContainer = document.getElementById('upload-progress-container');
    const noFilesMessage = document.getElementById('no-files-message');
    const pasteButton = document.getElementById('paste-button');
    const downloadAllZipButton = document.getElementById('download-all-zip-button');

    const selectAllCheckbox = document.getElementById('select-all-checkbox');



    // Modal elements
    const confirmationModalOverlay = document.getElementById('confirmation-modal-overlay');
    const confirmationModalMessage = document.getElementById('confirmation-modal-message');
    const modalConfirmButton = document.getElementById('modal-confirm-button');
    const modalCancelButton = document.getElementById('modal-cancel-button');
    const doNotAskAgainCheckbox = document.getElementById('do-not-ask-again');

    // --- Custom Confirmation Modal Logic ---
    let currentConfirmCallback = null;

    /**
     * Shows a custom confirmation modal.
     * @param {string} message The message to display in the modal.
     * @param {function} onConfirm Callback function to execute if the user confirms.
     */
    function showConfirmModal(message, onConfirm) {
        confirmationModalMessage.textContent = message;
        currentConfirmCallback = onConfirm; // Store the callback
        doNotAskAgainCheckbox.checked = false; // Reset checkbox state every time modal is opened

        confirmationModalOverlay.classList.add('active'); // Show modal

        // Ensure previous listeners are removed to prevent multiple calls
        modalConfirmButton.onclick = null;
        modalCancelButton.onclick = null;

        modalConfirmButton.onclick = () => {
            if (doNotAskAgainCheckbox.checked) {
                localStorage.setItem('doNotAskAgainDelete', 'true'); // Set preference
            }
            if (currentConfirmCallback) {
                currentConfirmCallback(true);
            }
            hideConfirmModal();
        };

        modalCancelButton.onclick = () => {
            if (currentConfirmCallback) {
                currentConfirmCallback(false); // Indicate cancellation
            }
            hideConfirmModal();
        };

        // Allow clicking outside to close
        confirmationModalOverlay.addEventListener('click', (event) => {
            if (event.target === confirmationModalOverlay) {
                if (currentConfirmCallback) {
                    currentConfirmCallback(false); // Indicate cancellation
                }
                hideConfirmModal();
            }
        }, { once: true }); // Use once to prevent multiple bindings
    }

    function hideConfirmModal() {
        confirmationModalOverlay.classList.remove('active');
        currentConfirmCallback = null; // Clear the callback
    }


    // --- File Listing ---
    async function fetchFiles() {
        try {
            const response = await fetch('/api/files');
            if (!response.ok) {
                const errorText = await response.text();
                console.error('Error fetching files:', response.status, errorText);
                filesTableBody.innerHTML = `<tr><td colspan="5" style="color: var(--error-color);">Error loading files: ${errorText}</td></tr>`;
                noFilesMessage.style.display = 'none';
                downloadAllZipButton.style.display = 'none'; // Hide button on error
                return;
            }
            const data = await response.json();
            renderFiles(data.files);
        } catch (error) {
            console.error('Failed to fetch files:', error);
            filesTableBody.innerHTML = `<tr><td colspan="5" style="color: var(--error-color);">Could not connect to server or error fetching files.</td></tr>`;
            noFilesMessage.style.display = 'none';
            downloadAllZipButton.style.display = 'none';

        }
    }

    function renderFiles(files) {
        filesTableBody.innerHTML = ''; // Clear existing files
        // clear selected
        selectAllCheckbox.checked = false;
        selectAllCheckbox.indeterminate = false;
        updateDownloadButtonLabel();

        if (!files || files.length === 0) {
            noFilesMessage.style.display = 'block';
            downloadAllZipButton.style.display = 'none'; // Hide button if no files
            return;
        }
        noFilesMessage.style.display = 'none';
        downloadAllZipButton.style.display = 'block';

        files.forEach(file => {
            const row = filesTableBody.insertRow();
            // Dynamically add data attributes for easy access
            row.dataset.fileName = file.name;

            // Checkbox:select
            const checkCell = row.insertCell();
            checkCell.dataset.label = 'Select';
            const checkbox = document.createElement('input');
            checkbox.type = 'checkbox';
            checkbox.className = 'file-select';
            checkbox.dataset.fileName = file.name;
            checkCell.appendChild(checkbox);

            // Add data-label attributes for responsive CSS
            const nameCell = row.insertCell();
            nameCell.textContent = file.name;
            nameCell.dataset.label = 'Name';

            const sizeCell = row.insertCell();
            sizeCell.textContent = file.formattedSize || formatBytes(file.size);
            sizeCell.dataset.label = 'Size';

            const modifiedCell = row.insertCell();
            modifiedCell.textContent = file.lastModified;
            modifiedCell.dataset.label = 'Last Modified';

            const typeCell = row.insertCell();
            typeCell.textContent = file.type;
            typeCell.dataset.label = 'Type';

            // Add the action icons container to the last cell
            const actionsCell = row.insertCell();
            actionsCell.dataset.label = 'Actions';
            actionsCell.style.textAlign = 'right'; // Align icons to the right

            const actionIconsContainer = document.createElement('div');
            actionIconsContainer.className = 'action-icons-container';

            // Download Icon
            const downloadLink = document.createElement('a');
            downloadLink.href = file.downloadUrl;
            downloadLink.setAttribute('download', file.name); // Suggest filename for download
            downloadLink.className = 'icon-button download-icon';
            downloadLink.title = `Download ${file.name}`;
            downloadLink.innerHTML = `
                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 -960 960 960">
                        <path d="M465-339.5q-7-2.5-13-8.5L308-492q-12-12-11.5-28t11.5-28q12-12 28.5-12.5T365-549l75 75v-286q0-17 11.5-28.5T480-800q17 0 28.5 11.5T520-760v286l75-75q12-12 28.5-11.5T652-548q11 12 11.5 28T652-492L508-348q-6 6-13 8.5t-15 2.5q-8 0-15-2.5ZM240-160q-33 0-56.5-23.5T160-240v-80q0-17 11.5-28.5T200-360q17 0 28.5 11.5T240-320v80h480v-80q0-17 11.5-28.5T760-360q17 0 28.5 11.5T800-320v80q0 33-23.5 56.5T720-160H240Z"/>
                    </svg>
                `;
            actionIconsContainer.appendChild(downloadLink);

            // Delete Icon
            const deleteButton = document.createElement('button');
            deleteButton.className = 'icon-button delete-icon';
            deleteButton.title = `Delete ${file.name}`;
            deleteButton.innerHTML = `
                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 -960 960 960">
                        <path d="M280-120q-33 0-56.5-23.5T200-200v-520q-17 0-28.5-11.5T160-760q0-17 11.5-28.5T200-800h160q0-17 11.5-28.5T400-840h160q17 0 28.5 11.5T600-800h160q17 0 28.5 11.5T800-760q0 17-11.5 28.5T760-720v520q0 33-23.5 56.5T680-120H280Zm400-600H280v520h400v-520ZM428.5-291.5Q440-303 440-320v-280q0-17-11.5-28.5T400-640q-17 0-28.5 11.5T360-600v280q0 17 11.5 28.5T400-280q17 0 28.5-11.5Zm160 0Q600-303 600-320v-280q0-17-11.5-28.5T560-640q-17 0-28.5 11.5T520-600v280q0 17 11.5 28.5T560-280q17 0 28.5-11.5ZM280-720v520-520Z"/>
                    </svg>
                `;
            deleteButton.onclick = (event) => {
                event.stopPropagation(); // Prevent row click from interfering if any
                confirmDeleteFile(file.name);
            };
            actionIconsContainer.appendChild(deleteButton);

            actionsCell.appendChild(actionIconsContainer);
        });
    }

    function confirmDeleteFile(fileName) {
        // Check localStorage preference first
        const doNotAskAgain = localStorage.getItem('doNotAskAgainDelete') === 'true';

        if (doNotAskAgain) {
            deleteFile(fileName); // Proceed directly if preference is set
        } else {
            showConfirmModal(`Delete "${fileName}"?`, (confirmed) => { // Shorter message
                if (confirmed) {
                    deleteFile(fileName);
                }
            });
        }
    }
    function showError(err) {
        const errorMsg = document.createElement('p');
        errorMsg.textContent = err;
        errorMsg.style.color = 'var(--error-color)';
        errorMsg.style.marginTop = '10px';
        errorMsg.style.textAlign = 'center';
        uploadProgressContainer.appendChild(errorMsg); // Temporary display area
        setTimeout(() => errorMsg.remove(), 5000); // Remove after 5 seconds

    }

    async function deleteFile(fileName) {
        try {
            const response = await fetch('/api/delete', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({ filename: fileName })
            });
            const result = await response.json();
            if (response.ok) {
                // Update the specific row
                const deletedRow = filesTableBody.querySelector(`[data-file-name="${fileName}"]`);
                if (deletedRow) {
                    deletedRow.remove();
                }
                if (filesTableBody.children.length === 0) {
                    noFilesMessage.style.display = 'block';
                    downloadAllZipButton.style.display = 'none';
                }
                updateDownloadButtonLabel()
                // Optionally show a temporary success message
                console.log(`Successfully deleted: ${fileName}`);
            } else {
                // Using a custom message display instead of alert
                console.error(`Error deleting file: ${result.error || 'Unknown error'}`);

                showError(
                    `Failed to delete ${fileName}: ${result.error || 'Unknown error'}`
                )

            }
        } catch (error) {
            console.error('Failed to send delete request:', error);
            showError(
                `Failed to send delete request for "${fileName}". Please check network.`
            )
        }
    }


    // --- Drag and Drop & File Upload ---
    dropZone.addEventListener('click', (event) => {
        if (event.target !== fileInput) { // only click if the click only fall under the dropzone
            event.stopPropagation();
            fileInput.click()
        }
    }
    );

    dropZone.addEventListener('dragover', (event) => {
        event.preventDefault();
        dropZone.classList.add('dragover');
    });

    dropZone.addEventListener('dragleave', () => {
        dropZone.classList.remove('dragover');
    });

    dropZone.addEventListener('drop', (event) => {
        event.preventDefault();
        dropZone.classList.remove('dragover');
        const files = event.dataTransfer.files;
        if (files.length > 0) {
            // validate there are no folders.
            if (!([...event.dataTransfer.items].every(item => item.webkitGetAsEntry()?.isFile))) {
                showError("Folders aren't supported. Compress them as ZIP first.");
                return
            }

            handleFiles(files);
        }
    });

    fileInput.addEventListener('change', (event) => {
        const files = event.target.files;
        if (files.length > 0) {
            handleFiles(files);
        }
        // Clear the file input so the same file can be selected again
        event.target.value = '';
    });

    function handleFiles(files) {
        Array.from(files).forEach(file => {
            uploadFile(file);
        });
    }

    function uploadFile(file) {
        const formData = new FormData();
        formData.append('file', file, file.name);

        const xhr = new XMLHttpRequest();

        // Create progress display for this file
        const progressItem = document.createElement('div');
        progressItem.className = 'progress-bar-item';
        const fileNameSpan = document.createElement('span');
        fileNameSpan.textContent = `${file.name} (${formatBytes(file.size)}): `;
        const progressBar = document.createElement('div');
        progressBar.className = 'progress-bar';
        const progressBarFill = document.createElement('div');
        progressBarFill.className = 'progress-bar-fill';
        const progressStatus = document.createElement('span');
        progressStatus.className = 'progress-bar-status';

        progressBar.appendChild(progressBarFill);
        progressItem.appendChild(fileNameSpan);
        progressItem.appendChild(progressBar);
        progressItem.appendChild(progressStatus);
        uploadProgressContainer.prepend(progressItem); // Add new progress at the top


        xhr.upload.addEventListener('progress', (event) => {
            if (event.lengthComputable) {
                const percentComplete = (event.loaded / event.total) * 100;
                progressBarFill.style.width = percentComplete.toFixed(2) + '%';
                progressStatus.textContent = `${percentComplete.toFixed(0)}%`;
            }
        });

        xhr.addEventListener('load', () => {
            if (xhr.status >= 200 && xhr.status < 300) {
                progressBarFill.style.backgroundColor = 'var(--success-color)';
                progressStatus.textContent = `Success: ${xhr.responseText}`;
                setTimeout(() => progressItem.remove(), 3000); // Remove success item after 3 seconds
                fetchFiles(); // Refresh file list after successful upload
            } else {
                progressBarFill.style.backgroundColor = 'var(--error-color)';
                progressStatus.textContent = `Error: ${xhr.status} - ${xhr.responseText || 'Upload failed'}`;
                console.error('Upload failed:', xhr.status, xhr.responseText);
                // Keep error message visible or provide a clear indication
            }
        });

        xhr.addEventListener('error', () => {
            progressBarFill.style.backgroundColor = 'var(--error-color)';
            progressStatus.textContent = 'Network Error';
            console.error('Upload error (network).');
        });

        xhr.open('POST', '/api/upload', true);
        xhr.send(formData);
    }

    /**
     * Formats bytes into a human-readable string (e.g., 1.23 MB).
     * @param {number} bytes The number of bytes.
     * @param {number} decimals The number of decimal places for the output.
     * @returns {string} Formatted size string.
     * This is, of course, GPT. If you said you have more than 1TB, I won't believe you, and YB is a trillion TB. :)
     */
    function formatBytes(bytes, decimals = 2) {
        if (bytes === 0) return '0 Bytes';
        const k = 1024;
        const dm = decimals < 0 ? 0 : decimals;
        const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
    }

    pasteButton.addEventListener('click', async () => {
        try {
            const text = await navigator.clipboard.readText();
            if (text.length > 0) {
                // Determine the next available filename
                let pasteIndex = 1;
                let fileName = `paste_${pasteIndex}.txt`;
                const existingFiles = Array.from(filesTableBody.querySelectorAll('[data-file-name]'))
                    .map(row => row.dataset.fileName);

                while (existingFiles.includes(fileName)) {
                    pasteIndex++;
                    fileName = `paste_${pasteIndex}.txt`;
                }

                const blob = new Blob([text], { type: 'text/plain' });
                const file = new File([blob], fileName, { type: 'text/plain', lastModified: new Date().getTime() });
                uploadFile(file, fileName); // Use the uploadFile function
            } else {
                showError('Clipboard is empty or contains no text.', 'info');
            }
        } catch (err) {
            console.error('Failed to read clipboard contents: ', err);
            showError('Failed to read clipboard. Please grant clipboard permissions.', 'error');
        }
    });
    function updateDownloadButtonLabel() {
        const all = filesTableBody.querySelectorAll('.file-select');
        const checkedCount = filesTableBody.querySelectorAll('.file-select:checked').length;

        downloadAllZipButton.textContent = (checkedCount === 0 || checkedCount === all.length)
            ? 'Download All as Zip'
            : `Download Selected (${checkedCount})`;

    }

    // select files
    selectAllCheckbox.addEventListener('change', () => {
        const all = filesTableBody.querySelectorAll('.file-select');
        all.forEach(cb => cb.checked = selectAllCheckbox.checked);
        updateDownloadButtonLabel();
    });
    filesTableBody.addEventListener('change', (e) => {
        if (!e.target.classList.contains('file-select')) return;

        const all = filesTableBody.querySelectorAll('.file-select');
        const checked = filesTableBody.querySelectorAll('.file-select:checked');

        selectAllCheckbox.checked = (all.length === checked.length);
        selectAllCheckbox.indeterminate =
            checked.length > 0 && checked.length < all.length;

        updateDownloadButtonLabel();
    });
    const downloadZip = (files) => {
        //this is a workaround to make the user download when the request start
        const form = document.createElement('form');
        form.method = 'POST';
        form.action = '/api/zip';
        form.style.display = 'none';

        files.forEach(file => {
            const input = document.createElement('input');
            input.type = 'hidden';
            input.name = 'f';
            input.value = file;
            form.appendChild(input);
        });

        document.body.appendChild(form);
        form.submit();
        document.body.removeChild(form);
    };

    downloadAllZipButton.addEventListener("click", () => {
        const selectedCheckboxes = filesTableBody.querySelectorAll(
            ".file-select:checked",
        );
        const files = [...selectedCheckboxes].map(cb => cb.dataset.fileName)
        downloadZip(files)

    });

    // Initial load of files when the page is ready
    fetchFiles();
});