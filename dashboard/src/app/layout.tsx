// مسیر: dashboard/src/app/layout.tsx
import './globals.css';
import { TerminalProvider } from '../context/TerminalContext';

export const metadata = {
  title: 'MI-EDTE Quant Intelligence & Trading Terminal',
  description: '100% Real-Time High-Frequency Scalping & Audited Statement Engine',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="min-h-screen bg-slate-950 text-slate-100">
        <TerminalProvider>
          {children}
        </TerminalProvider>
      </body>
    </html>
  );
}