import { createBrowserRouter, Navigate, Outlet } from 'react-router-dom';
import { useAuth } from '@/lib/auth/context';
import { MainLayout } from '@/components/layout/MainLayout';
import { LoginPage } from '@/features/auth/LoginPage';
import { DashboardPage } from '@/features/dashboard/DashboardPage';
import { NodesPage } from '@/features/nodes/NodesPage';
import { NodeDetailPage } from '@/features/nodes/NodeDetailPage';
import { NetworksPage } from '@/features/networks/NetworksPage';
import { StoragePoolsPage } from '@/features/storage-pools/StoragePoolsPage';
import { ImagesPage } from '@/features/images/ImagesPage';
import { VMsPage } from '@/features/vms/VMsPage';
import { VMDetailPage } from '@/features/vms/VMDetailPage';
import { VMCreatePage } from '@/features/vms/VMCreatePage';
import { OperationsPage } from '@/features/operations/OperationsPage';

// Protected route wrapper
function ProtectedRoute() {
  const { isAuthenticated } = useAuth();
  
  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }
  
  return (
    <MainLayout>
      <Outlet />
    </MainLayout>
  );
}

// Public only route wrapper (for login page)
function PublicOnlyRoute() {
  const { isAuthenticated } = useAuth();
  
  if (isAuthenticated) {
    return <Navigate to="/" replace />;
  }
  
  return <Outlet />;
}

export const router = createBrowserRouter([
  {
    element: <PublicOnlyRoute />,
    children: [
      {
        path: '/login',
        element: <LoginPage />,
      },
    ],
  },
  {
    element: <ProtectedRoute />,
    children: [
      {
        path: '/',
        element: <DashboardPage />,
      },
      {
        path: '/nodes',
        element: <NodesPage />,
      },
      {
        path: '/nodes/:id',
        element: <NodeDetailPage />,
      },
      {
        path: '/networks',
        element: <NetworksPage />,
      },
      {
        path: '/storage-pools',
        element: <StoragePoolsPage />,
      },
      {
        path: '/images',
        element: <ImagesPage />,
      },
      {
        path: '/vms',
        element: <VMsPage />,
      },
      {
        path: '/vms/new',
        element: <VMCreatePage />,
      },
      {
        path: '/vms/:id',
        element: <VMDetailPage />,
      },
      {
        path: '/operations',
        element: <OperationsPage />,
      },
    ],
  },
  {
    path: '*',
    element: <Navigate to="/" replace />,
  },
]);
