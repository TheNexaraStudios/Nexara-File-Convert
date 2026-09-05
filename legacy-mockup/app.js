// Basic JavaScript for the application
document.addEventListener('DOMContentLoaded', () => {
    const uploadArea = document.getElementById('uploadArea');
    const fileList = document.getElementById('fileList');
    const convertBtn = document.getElementById('convertBtn');
    const conversionList = document.getElementById('conversionList');

    uploadArea.addEventListener('dragover', (e) => {
        e.preventDefault();
        uploadArea.style.borderColor = 'var(--accent)';
    });

    uploadArea.addEventListener('dragleave', () => {
        uploadArea.style.borderColor = 'var(--border)';
    });

    uploadArea.addEventListener('drop', (e) => {
        e.preventDefault();
        uploadArea.style.borderColor = 'var(--border)';
        handleFiles(e.dataTransfer.files);
    });

    uploadArea.addEventListener('click', () => {
        document.createElement('input').click();
    });

    document.addEventListener('change', (e) => {
        if (e.target.type === 'file') {
            handleFiles(e.target.files);
        }
    });

    convertBtn.addEventListener('click', () => {
        // Start conversion process
        startConversion();
    });

    function handleFiles(files) {
        const maxFileSize = 500 * 1024 * 1024; // 500 MB
        for (const file of files) {
            if (file.size > maxFileSize) {
                alert(`File "${file.name}" is too large. Max size is 500 MB.`);
                continue;
            }
            const fileItem = document.createElement('div');
            fileItem.className = 'file-item';
            fileItem.innerHTML = `<span>${file.name}</span>`;
            fileList.appendChild(fileItem);
        }
    }

    function startConversion() {
        // Simulate conversion process
        convertBtn.disabled = true;
        convertBtn.textContent = 'Converting...';

        setTimeout(() => {
            convertBtn.disabled = false;
            convertBtn.textContent = 'Convert Files';
            addConversionToList('Success');
        }, 3000);
    }

    function addConversionToList(status) {
        const listItem = document.createElement('li');
        listItem.className = 'conversion-item';
        listItem.textContent = `Conversion ${status}`;
        conversionList.appendChild(listItem);
    }
});
