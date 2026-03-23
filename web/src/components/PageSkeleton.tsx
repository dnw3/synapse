import { LoadingSkeleton } from "./dashboard/shared";

export default function PageSkeleton() {
  return (
    <div className="flex flex-col gap-6 p-6 animate-fade-in">
      <div className="flex items-center justify-between">
        <LoadingSkeleton className="h-8 w-48" />
        <LoadingSkeleton className="h-8 w-24" />
      </div>
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {[0, 1, 2, 3].map((i) => (
          <LoadingSkeleton key={i} className="h-28 w-full" />
        ))}
      </div>
      <LoadingSkeleton className="h-64 w-full" />
    </div>
  );
}
