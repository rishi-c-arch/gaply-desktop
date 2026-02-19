import React from 'react';
import { ArrowRight, ArrowDown } from 'lucide-react';
import type { Project } from '../../types/dashboard';

interface ProjectsTableProps {
  projects?: Project[];
}

const ProjectsTable: React.FC<ProjectsTableProps> = ({ projects = [] }) => {
  return (
    <div
      style={{
        background: 'var(--dashboard-card-bg)',
        borderRadius: 12,
        border: '1px solid var(--dashboard-border)',
        padding: 24,
        marginBottom: 24,
      }}
    >
        <h3 style={{ fontSize: 16, fontWeight: 600, color: 'var(--dashboard-text)', margin: '0 0 20px 0' }}>
        Recent Projects
      </h3>

      <table style={{ width: '100%', borderCollapse: 'collapse' }}>
        <thead>
          <tr>
            <th
              style={{
                textAlign: 'left',
                padding: '12px 0',
                fontSize: 12,
                fontWeight: 500,
                color: 'var(--dashboard-text-muted)',
              }}
            >
              Name
            </th>
            <th
              style={{
                textAlign: 'left',
                padding: '12px 0',
                fontSize: 12,
                fontWeight: 500,
                color: 'var(--dashboard-text-muted)',
              }}
            >
              <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                Date
                <ArrowDown size={14} />
              </span>
            </th>
            <th
              style={{
                textAlign: 'left',
                padding: '12px 0',
                fontSize: 12,
                fontWeight: 500,
                color: 'var(--dashboard-text-muted)',
              }}
            >
              Status
            </th>
            <th style={{ width: 40 }} />
          </tr>
        </thead>
        <tbody>
          {projects.length === 0 ? (
            <tr>
              <td colSpan={4} style={{ padding: 24, textAlign: 'center', color: 'var(--dashboard-text-muted)', fontSize: 14 }}>
                No projects yet
              </td>
            </tr>
          ) : (
          projects.map((project) => (
            <tr
              key={project.id}
              style={{
                borderTop: '1px solid var(--dashboard-border)',
                cursor: 'pointer',
                transition: 'background 150ms ease',
              }}
            >
              <td style={{ padding: '14px 0', fontSize: 14, color: 'var(--dashboard-text)', fontWeight: 500 }}>
                {project.name}
              </td>
              <td style={{ padding: '14px 0', fontSize: 14, color: 'var(--dashboard-text-muted)' }}>{project.date}</td>
              <td style={{ padding: '14px 0' }}>
                <span style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <span
                    style={{
                      width: 8,
                      height: 8,
                      borderRadius: '50%',
                      background:
                        project.status?.toLowerCase() === 'complete'
                          ? 'var(--dashboard-success)'
                          : project.status?.toLowerCase() === 'failed'
                            ? 'var(--dashboard-error, #ef4444)'
                            : 'var(--dashboard-warning)',
                    }}
                  />
                  <span
                    style={{
                      fontSize: 14,
                      color:
                        project.status?.toLowerCase() === 'complete'
                          ? 'var(--dashboard-success)'
                          : project.status?.toLowerCase() === 'failed'
                            ? 'var(--dashboard-error, #ef4444)'
                            : 'var(--dashboard-warning)',
                    }}
                  >
                    {project.status || 'Running'}
                  </span>
                </span>
              </td>
              <td style={{ padding: '14px 0' }}>
                <ArrowRight size={16} color="var(--dashboard-text-muted)" />
              </td>
            </tr>
          )))}
        </tbody>
      </table>
    </div>
  );
};

export default ProjectsTable;
