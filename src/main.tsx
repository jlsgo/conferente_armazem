import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { ToastProvider } from './lib/toast';
import { DialogoProvider } from './lib/dialogo';
import './styles/global.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ToastProvider>
      <DialogoProvider>
        <App />
      </DialogoProvider>
    </ToastProvider>
  </React.StrictMode>
);
