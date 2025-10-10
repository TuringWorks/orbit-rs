import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';

// Prevent right-click context menu in Tauri app
document.addEventListener('contextmenu', e => e.preventDefault());

// Prevent drag and drop which can interfere with Tauri
document.addEventListener('dragover', e => e.preventDefault());
document.addEventListener('drop', e => e.preventDefault());

// Initialize the React application
ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);