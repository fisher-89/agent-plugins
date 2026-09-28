import ReactDOM from 'react-dom/client';

import App from './app';

import './styles/global.css';

const rootElement = document.getElementById('root');
if (rootElement) {
  ReactDOM.createRoot(rootElement).render(<App />);
}
