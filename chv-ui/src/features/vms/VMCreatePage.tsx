import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useMutation } from '@tanstack/react-query';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { z } from 'zod';
import { vmsApi, nodesApi, imagesApi, networksApi } from '@/lib/api';
import { ArrowLeft, ChevronRight, ChevronLeft, Cpu } from 'lucide-react';
import type { VMCreateRequest } from '@/types';

const vmCreateSchema = z.object({
  name: z.string().min(1, 'Name is required').max(128, 'Name too long'),
  node_id: z.string().optional(),
  image_id: z.string().min(1, 'Image is required'),
  vcpu: z.number().min(1, 'At least 1 vCPU').max(64, 'Max 64 vCPUs'),
  memory_mb: z.number().min(512, 'At least 512 MB').max(524288, 'Max 512 GB'),
  disk_size_gb: z.number().min(5, 'At least 5 GB').max(2048, 'Max 2048 GB'),
  network_id: z.string().min(1, 'Network is required'),
  user_data: z.string().optional(),
});

type VMCreateForm = z.infer<typeof vmCreateSchema>;

export function VMCreatePage() {
  const navigate = useNavigate();
  const [step, setStep] = useState(1);
  
  const { data: nodes } = useQuery({
    queryKey: ['nodes'],
    queryFn: () => nodesApi.list(),
  });
  
  const { data: images } = useQuery({
    queryKey: ['images'],
    queryFn: () => imagesApi.list(),
  });
  
  const { data: networks } = useQuery({
    queryKey: ['networks'],
    queryFn: () => networksApi.list(),
  });

  const createVM = useMutation({
    mutationFn: (data: VMCreateRequest) => vmsApi.create(data),
    onSuccess: (vm) => {
      navigate(`/vms/${vm.id}`);
    },
  });

  const {
    register,
    handleSubmit,
    formState: { errors },
  } = useForm<VMCreateForm>({
    resolver: zodResolver(vmCreateSchema),
    defaultValues: {
      vcpu: 2,
      memory_mb: 2048,
      disk_size_gb: 20,
    },
  });

  const onSubmit = (data: VMCreateForm) => {
    const request: VMCreateRequest = {
      name: data.name,
      vcpu: data.vcpu,
      memory_mb: data.memory_mb,
      disk_size_bytes: data.disk_size_gb * 1024 * 1024 * 1024,
      image_id: data.image_id,
      networks: [{ network_id: data.network_id }],
      cloud_init: data.user_data ? {
        user_data: data.user_data,
      } : undefined,
    };
    createVM.mutate(request);
  };

  const readyImages = images?.items?.filter((img) => img.status === 'ready') || [];

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <button
          onClick={() => navigate('/vms')}
          className="p-2 rounded-md hover:bg-accent"
        >
          <ArrowLeft className="h-5 w-5" />
        </button>
        <div>
          <h1 className="text-2xl font-bold text-foreground">Create Virtual Machine</h1>
          <p className="text-muted-foreground">Step {step} of 3</p>
        </div>
      </div>

      {/* Progress */}
      <div className="flex gap-2">
        {[1, 2, 3].map((s) => (
          <div
            key={s}
            className={`h-2 flex-1 rounded-full ${
              s <= step ? 'bg-primary' : 'bg-muted'
            }`}
          />
        ))}
      </div>

      <form onSubmit={handleSubmit(onSubmit)} className="bg-card border border-border rounded-lg p-6">
        {step === 1 && (
          <div className="space-y-4">
            <h2 className="text-lg font-semibold">Basic Configuration</h2>
            
            <div>
              <label className="block text-sm font-medium mb-1">VM Name</label>
              <input
                {...register('name')}
                className="w-full px-3 py-2 border border-input rounded-md bg-background"
                placeholder="my-vm"
              />
              {errors.name && (
                <p className="text-sm text-destructive mt-1">{errors.name.message}</p>
              )}
            </div>

            <div>
              <label className="block text-sm font-medium mb-1">Image</label>
              <select
                {...register('image_id')}
                className="w-full px-3 py-2 border border-input rounded-md bg-background"
              >
                <option value="">Select an image</option>
                {readyImages.map((img) => (
                  <option key={img.id} value={img.id}>
                    {img.name} ({img.os_family}, {img.architecture})
                  </option>
                ))}
              </select>
              {errors.image_id && (
                <p className="text-sm text-destructive mt-1">{errors.image_id.message}</p>
              )}
            </div>

            <div>
              <label className="block text-sm font-medium mb-1">Node (Optional)</label>
              <select
                {...register('node_id')}
                className="w-full px-3 py-2 border border-input rounded-md bg-background"
              >
                <option value="">Auto-select</option>
                {nodes?.items?.map((node) => (
                  <option key={node.id} value={node.id}>
                    {node.hostname}
                  </option>
                ))}
              </select>
            </div>
          </div>
        )}

        {step === 2 && (
          <div className="space-y-4">
            <h2 className="text-lg font-semibold">Compute & Storage</h2>
            
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium mb-1">vCPUs</label>
                <input
                  type="number"
                  {...register('vcpu', { valueAsNumber: true })}
                  className="w-full px-3 py-2 border border-input rounded-md bg-background"
                />
                {errors.vcpu && (
                  <p className="text-sm text-destructive mt-1">{errors.vcpu.message}</p>
                )}
              </div>

              <div>
                <label className="block text-sm font-medium mb-1">Memory (MB)</label>
                <input
                  type="number"
                  {...register('memory_mb', { valueAsNumber: true })}
                  className="w-full px-3 py-2 border border-input rounded-md bg-background"
                />
                {errors.memory_mb && (
                  <p className="text-sm text-destructive mt-1">{errors.memory_mb.message}</p>
                )}
              </div>
            </div>

            <div>
              <label className="block text-sm font-medium mb-1">Disk Size (GB)</label>
              <input
                type="number"
                {...register('disk_size_gb', { valueAsNumber: true })}
                className="w-full px-3 py-2 border border-input rounded-md bg-background"
              />
              {errors.disk_size_gb && (
                <p className="text-sm text-destructive mt-1">{errors.disk_size_gb.message}</p>
              )}
            </div>
          </div>
        )}

        {step === 3 && (
          <div className="space-y-4">
            <h2 className="text-lg font-semibold">Network & Cloud-init</h2>
            
            <div>
              <label className="block text-sm font-medium mb-1">Network</label>
              <select
                {...register('network_id')}
                className="w-full px-3 py-2 border border-input rounded-md bg-background"
              >
                <option value="">Select a network</option>
                {networks?.items?.map((net) => (
                  <option key={net.id} value={net.id}>
                    {net.name} ({net.cidr})
                  </option>
                ))}
              </select>
              {errors.network_id && (
                <p className="text-sm text-destructive mt-1">{errors.network_id.message}</p>
              )}
            </div>

            <div>
              <label className="block text-sm font-medium mb-1">
                Cloud-init User Data (Optional)
              </label>
              <textarea
                {...register('user_data')}
                rows={6}
                className="w-full px-3 py-2 border border-input rounded-md bg-background font-mono text-sm"
                placeholder="#cloud-config\nusers:\n  - name: admin\n    sudo: ALL=(ALL) NOPASSWD:ALL"
              />
            </div>
          </div>
        )}

        {/* Navigation */}
        <div className="flex justify-between mt-6 pt-6 border-t border-border">
          <button
            type="button"
            onClick={() => setStep(Math.max(1, step - 1))}
            disabled={step === 1}
            className="flex items-center gap-2 px-4 py-2 border border-border rounded-md text-sm font-medium hover:bg-accent disabled:opacity-50"
          >
            <ChevronLeft className="h-4 w-4" />
            Back
          </button>
          
          {step < 3 ? (
            <button
              type="button"
              onClick={() => setStep(step + 1)}
              className="flex items-center gap-2 px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm font-medium hover:bg-primary/90"
            >
              Next
              <ChevronRight className="h-4 w-4" />
            </button>
          ) : (
            <button
              type="submit"
              disabled={createVM.isPending}
              className="flex items-center gap-2 px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm font-medium hover:bg-primary/90 disabled:opacity-50"
            >
              <Cpu className="h-4 w-4" />
              {createVM.isPending ? 'Creating...' : 'Create VM'}
            </button>
          )}
        </div>

        {createVM.error && (
          <div className="mt-4 p-4 bg-destructive/10 border border-destructive/20 rounded-md">
            <p className="text-sm text-destructive">
              Failed to create VM: {(createVM.error as Error).message}
            </p>
          </div>
        )}
      </form>
    </div>
  );
}
