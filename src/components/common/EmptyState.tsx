interface EmptyStateProps {
  icon?: string;
  title: string;
  description: string;
  action?: {
    label: string;
    onClick: () => void;
  };
}

export function EmptyState({ icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center h-full text-center p-8">
      {icon && <span className="text-5xl mb-4">{icon}</span>}
      <h3 className="text-xl font-semibold text-zinc-200 mb-2">{title}</h3>
      <p className="text-zinc-400 mb-6 max-w-md">{description}</p>
      {action && (
        <button
          onClick={action.onClick}
          className="px-4 py-2 bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
        >
          {action.label}
        </button>
      )}
    </div>
  );
}
