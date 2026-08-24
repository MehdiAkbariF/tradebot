import './globals.css';


export const metadata = {
  title: 'MI-EDTE Trading Intelligence Dashboard',
  description: 'Real-time Market Intelligence & Event-Driven Trading System',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="min-h-screen bg-slate-950 text-slate-100">{children}</body>
    </html>
  );
}